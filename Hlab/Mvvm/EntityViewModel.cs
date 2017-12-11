/*
  HLab.Mvvm
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Mvvm.

    HLab.Mvvm is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Mvvm is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System.ComponentModel;
using Hlab.Base;
using Hlab.Notify;

//using System.Data.Model;

namespace Hlab.Mvvm
{
    public static class EntityExt
    {
        public static TEntityViewModel ViewModel<TEntityViewModel, T>(this T entity)
            where TEntityViewModel : IViewModel<T>, new()
        {
            var viewModel = new TEntityViewModel();
            viewModel.SetModel(entity);
            return viewModel;
        }
        public static object GetModel(this INotifyPropertyChanged viewModel) => viewModel.Get<object>("Model");
        public static T GetModel<T>(this IViewModel<T> viewModel) => viewModel.Get<T>("Model");

        public static void SetModel<T>(this INotifyPropertyChanged viewModel, T model)
        {
            using (viewModel.Suspend())
            {
                var token = viewModel.Get<SuspenderToken>("EntityNullToken");
                if (token != null && model != null) token.Dispose();
                if (viewModel.Set(model, "Model"))
                {
                    if (model == null) viewModel.Set<SuspenderToken>(viewModel.Suspend(), "EntityNullToken");
                }
            }
        }
        public static T GetLinked<T>(this INotifyPropertyChanged viewModel) => viewModel.Get<T>("Linked");
        public static void SetLinked<T>(this INotifyPropertyChanged viewModel, T view)
        {
            using (viewModel.Suspend())
            {
                var token = viewModel.Get<SuspenderToken>("EntityNullToken");
                if (token != null && viewModel != null) token.Dispose();
                if (viewModel.Set(view, "Linked"))
                {
                    //if (model == null) viewModel.Set<SuspenderToken>(viewModel.Suspend(), "EntityNullToken");
                }
            }
        }

        //public static void SetModel<T,TViewModel>(this TViewModel viewModel, T model)
        //    where TViewModel : INotifyPropertyChanged, IViewModel<T>
        //{
        //    ((INotifyPropertyChanged)viewModel).SetModel(model);
        //}

        public static bool IsDbLinked(this INotifyPropertyChanged viewModel) => true;
        




    }

    //public interface IViewModel : INotifyPropertyChanged
    //{
    //}

    public interface IViewModel<out T> : INotifyPropertyChanged
    //where T : INotifyPropertyChanged
    {
        T Model { get; }
    }
}
