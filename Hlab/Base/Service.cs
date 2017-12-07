namespace Hlab.Base
{
    public interface IService { }


    //public class Service
    //{
    //    private static readonly ConcurrentDictionary<Type, IService> Services = new ConcurrentDictionary<Type, IService>();

    //    public static T Get<T>()
    //        where T :IService, new()
    //    {
    //        return (T)Services.GetOrAdd(typeof(T), e => new T());
    //    }

    //    public static IService Get(Type type)
    //    {
    //        return Services.GetOrAdd(type, t => (IService)Activator.CreateInstance(t) );          
    //    }
    //}
}
